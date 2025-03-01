use crate::deriving::generic::ty::*;
use crate::deriving::generic::*;
use crate::deriving::path_std;

use rustc_ast::MetaItem;
use rustc_expand::base::{Annotatable, ExtCtxt};
use rustc_span::symbol::{sym, Ident};
use rustc_span::Span;

pub fn expand_deriving_ord(
    cx: &mut ExtCtxt<'_>,
    span: Span,
    mitem: &MetaItem,
    item: &Annotatable,
    push: &mut dyn FnMut(Annotatable),
) {
    let inline = cx.meta_word(span, sym::inline);
    let attrs = vec![cx.attribute(inline)];
    let trait_def = TraitDef {
        span,
        attributes: Vec::new(),
        path: path_std!(cmp::Ord),
        additional_bounds: Vec::new(),
        generics: Bounds::empty(),
        supports_unions: false,
        methods: vec![MethodDef {
            name: sym::cmp,
            generics: Bounds::empty(),
            explicit_self: true,
            nonself_args: vec![(self_ref(), sym::other)],
            ret_ty: Path(path_std!(cmp::Ordering)),
            attributes: attrs,
            unify_fieldless_variants: true,
            combine_substructure: combine_substructure(Box::new(|a, b, c| cs_cmp(a, b, c))),
        }],
        associated_types: Vec::new(),
    };

    trait_def.expand(cx, mitem, item, push)
}

pub fn cs_cmp(cx: &mut ExtCtxt<'_>, span: Span, substr: &Substructure<'_>) -> BlockOrExpr {
    let test_id = Ident::new(sym::cmp, span);
    let equal_path = cx.path_global(span, cx.std_path(&[sym::cmp, sym::Ordering, sym::Equal]));
    let cmp_path = cx.std_path(&[sym::cmp, sym::Ord, sym::cmp]);

    // Builds:
    //
    // match ::core::cmp::Ord::cmp(&self.x, &other.x) {
    //     ::std::cmp::Ordering::Equal =>
    //         ::core::cmp::Ord::cmp(&self.y, &other.y),
    //     cmp => cmp,
    // }
    let expr = cs_fold(
        // foldr nests the if-elses correctly, leaving the first field
        // as the outermost one, and the last as the innermost.
        false,
        cx,
        span,
        substr,
        |cx, fold| match fold {
            CsFold::Single(field) => {
                let [other_expr] = &field.other_selflike_exprs[..] else {
                        cx.span_bug(field.span, "not exactly 2 arguments in `derive(Ord)`");
                    };
                let args = vec![
                    cx.expr_addr_of(field.span, field.self_expr.clone()),
                    cx.expr_addr_of(field.span, other_expr.clone()),
                ];
                cx.expr_call_global(field.span, cmp_path.clone(), args)
            }
            CsFold::Combine(span, expr1, expr2) => {
                let eq_arm = cx.arm(span, cx.pat_path(span, equal_path.clone()), expr1);
                let neq_arm =
                    cx.arm(span, cx.pat_ident(span, test_id), cx.expr_ident(span, test_id));
                cx.expr_match(span, expr2, vec![eq_arm, neq_arm])
            }
            CsFold::Fieldless => cx.expr_path(equal_path.clone()),
            CsFold::EnumNonMatching(span, tag_tuple) => {
                if tag_tuple.len() != 2 {
                    cx.span_bug(span, "not exactly 2 arguments in `derive(Ord)`")
                } else {
                    let lft = cx.expr_addr_of(span, cx.expr_ident(span, tag_tuple[0]));
                    let rgt = cx.expr_addr_of(span, cx.expr_ident(span, tag_tuple[1]));
                    let fn_cmp_path = cx.std_path(&[sym::cmp, sym::Ord, sym::cmp]);
                    cx.expr_call_global(span, fn_cmp_path, vec![lft, rgt])
                }
            }
        },
    );
    BlockOrExpr::new_expr(expr)
}
