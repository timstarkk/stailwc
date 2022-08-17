#![feature(box_patterns)]

#[cfg(test)]
mod test;

use itertools::{Either, Itertools};
use swc_ecmascript::ast::{
    Expr, Ident, JSXAttr, JSXAttrName, JSXAttrValue, JSXExpr, JSXExprContainer, Lit, Program, Str,
};

use swc_common::{errors::HANDLER, MultiSpan, Span, DUMMY_SP};
use swc_ecma_visit::{
    as_folder,
    swc_ecma_ast::{
        JSXAttrOrSpread, JSXOpeningElement, KeyValueProp, ObjectLit, Prop, PropName, PropOrSpread,
    },
    FoldWith, VisitMut, VisitMutWith,
};
use swc_plugin::metadata::TransformPluginProgramMetadata;
use swc_plugin_macro::plugin_transform;
use tracing::error;

#[derive(Default)]
pub struct TransformVisitor {
    css_attr: Option<ObjectLit>,
}

/**
 * Implement necessary visit_mut_* methods for actual custom transform.
 * A comprehensive list of possible visitor methods can be found here:
 * https://rustdoc.swc.rs/swc_ecma_visit/trait.VisitMut.html
 */
impl VisitMut for TransformVisitor {
    /**
     * Handle jsx attributes and convert them into emotion.
     */
    fn visit_mut_jsx_attr(&mut self, n: &mut JSXAttr) {
        let sym = match &n.name {
            JSXAttrName::Ident(Ident { sym, .. }) if sym == "tw" => "tw",
            _ => return,
        };

        match &n.value {
            // tw="h-4"
            Some(JSXAttrValue::Lit(Lit::Str(string)))
            // tw={"h-4"}
            | Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                expr: JSXExpr::Expr(box Expr::Lit(Lit::Str(string))),
                ..
            })) => {
                if let Some(x) = self.css_attr.replace(generate_css_prop(&string.value)) {
                    println!("warn : encountered multiple tw attributes");
                }
            }
            Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                expr: JSXExpr::Expr(box Expr::Ident(Ident { sym, .. })),
                span,
                ..
            })) => {
                // todo(arlyon): enable error reporting
                // HANDLER.with(|h| {
                //         h.struct_span_warn(MultiSpan::from_span(*span), "variables are not supported")
                //             .emit()
                // });
                println!("fail : variables are not supported `{}`", sym);
            }
            _ => println!("fail : unknown pattern {:#?}", n),
        }
    }

    /**
     * Visit children to extract any tailwind attributes, then:
     * - convert them into emotion
     * - remove any tailwind attributes
     */
    fn visit_mut_jsx_opening_element(&mut self, n: &mut JSXOpeningElement) {
        n.attrs.visit_mut_children_with(self);

        let lit = match self.css_attr.take() {
            Some(v) => v,
            _ => return,
        };

        println!("parsed: {:?}", lit);

        n.attrs.retain(|attr| match &attr {
            swc_ecma_visit::swc_ecma_ast::JSXAttrOrSpread::JSXAttr(JSXAttr {
                name: JSXAttrName::Ident(Ident { sym, .. }),
                ..
            }) if sym == "tw" => false,
            _ => true,
        });

        n.attrs.push(JSXAttrOrSpread::JSXAttr(JSXAttr {
            name: JSXAttrName::Ident(Ident {
                sym: "css".into(),
                span: DUMMY_SP,
                optional: false,
            }),
            span: DUMMY_SP,
            value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
                expr: JSXExpr::Expr(Box::new(Expr::Object(lit))),
                span: DUMMY_SP,
            })),
        }));
    }
}

fn generate_css_prop(tw: &str) -> ObjectLit {
    let props = tw.split(' ').filter_map(|s| {
        let (k, v) = match s {
            "h-4" => Some(("height", "4em")),
            "text-red" => Some(("color", "red")),
            "bg-red" => Some(("background-color", "red")),
            x => None,
        }?;

        Some(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
            key: PropName::Str(Str {
                span: DUMMY_SP,
                raw: None,
                value: k.into(),
            }),
            value: Box::new(Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                raw: None,
                value: v.into(),
            }))),
        }))))
    });

    ObjectLit {
        span: DUMMY_SP,
        props: props.collect(),
    }
}

#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::default()))
}
