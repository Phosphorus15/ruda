use std::iter::Extend;
use std::collections::HashMap;
use pest::iterators::Pair;
use std::ops::Add;

#[derive(Parser)]
#[grammar = "ruda.pest"]
pub struct RudaParser;

pub(crate) type RuleList<'a> = Vec<Pair<'a, Rule>>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TyName {
    NameBind(String),
    VarBind(String),
    MutBind(Box<Self>),
    Array(Box<Self>),
    Arrow(Box<Self>, Box<Self>),
    Tuple(Vec<Self>),
    Unit,
}

impl TyName {
    fn get_arrow(params: Vec<TyName>, ret: TyName) -> TyName {
        TyName::Arrow(Box::new(TyName::Tuple(params)), Box::new(ret))
    }
}

pub type TypedExpr = (BaseExpr, TyName);

#[derive(Debug, Clone)]
pub enum BaseExpr {
    FuncDecl { ident: String, para_in: Vec<String>, is_par: bool, params: Vec<(String, TyName)>, ret: TyName, body: Vec<TypedExpr> },
    FuncCall(String, Vec<TypedExpr>),
    LetDecl(String, bool, Box<TypedExpr>),
    Assign(String, Box<TypedExpr>),
    Return(Box<TypedExpr>),
    RetNull,
    Ident(String),
    ConstantFloat(f64),
    ConstantInt(i64),
    Nope,
}

fn gen_bin_op(op: &String, lhs: TypedExpr, rhs: TypedExpr) -> TypedExpr {
    let name = String::from("@").add(&op[..]);
    (BaseExpr::FuncCall(name.clone(), vec![lhs, rhs]),
     TyName::VarBind(format!("ret@{}", name)))
}

pub fn walk_pairs(pairs: pest::iterators::Pairs<Rule>) -> Vec<TypedExpr> {
    pairs.into_iter().map(walk_func).collect()
}

macro_rules! parse_param {
( $ x: expr) => {$x.map( | item| {
let pair: Vec < _ > = item.into_inner().collect();
return (pair[0].as_span().as_str().to_string(), walk_ty(pair[1].clone()));
}).collect()
}
}

fn walk_func(func: Pair<Rule>) -> TypedExpr {
    let vec: RuleList = func.into_inner().collect();
    let decl: RuleList = vec[0].clone().into_inner().collect();
    let body: RuleList = vec[1].clone().into_inner().collect();
    let mut para_in: Vec<String> = vec![];
    let ret_type = if decl.last().unwrap().as_rule() == Rule::ret_type {
        walk_ty(decl.last().unwrap().clone())
    } else {
        TyName::Unit
    };
    if decl[0].as_rule() == Rule::parfun_decl {
        para_in = decl[0].clone().into_inner().next().unwrap().clone().into_inner().map(|i| i.as_span().as_str().to_string()).collect();
        let name = decl[1].as_span().as_str().to_string();
        let params: Vec<_> = parse_param!(decl[2].clone().into_inner());
        return (BaseExpr::FuncDecl {
            ident: name,
            para_in,
            params: params.clone(),
            ret: ret_type.clone(),
            is_par: true,
            body: walk_fun_body(body),
        }, TyName::get_arrow(params.into_iter().map(|v| v.1).collect(), ret_type));
    } else {
        let name = decl[0].as_span().as_str().to_string();
        let params: Vec<_> = parse_param!(decl[1].clone().into_inner());
        return (BaseExpr::FuncDecl {
            ident: name,
            para_in,
            params: params.clone(),
            ret: ret_type.clone(),
            is_par: false,
            body: walk_fun_body(body),
        }, TyName::get_arrow(params.into_iter().map(|v| v.1).collect(), ret_type));
    }
}

fn walk_fun_body(body: RuleList) -> Vec<TypedExpr> {
    body.into_iter().map(|expr| {
        if expr.as_rule() == Rule::base_expr {
            let inner = expr.into_inner().take(1).collect::<RuleList>()[0].clone();
            return match inner.as_rule() {
                Rule::return_expr => {
                    let vec = inner.into_inner().collect::<RuleList>();
                    if vec.len() == 0 {
                        (BaseExpr::RetNull, TyName::Unit)
                    } else {
                        let value_expr = vec[0].clone();
                        let walked_val = walk_value_expr(value_expr.into_inner().collect());
                        (BaseExpr::Return(Box::new(walked_val.clone())), walked_val.1)
                    }
                }
                Rule::let_expr => {
                    let composition = inner.into_inner().collect::<RuleList>();
                    let mut base = 0;
                    if composition[base].as_rule() == Rule::mut_let {
                        base = 1;
                    }
                    let id = composition[base].as_str().to_string();
                    let val = walk_value_expr(composition[base + 1].clone().into_inner().collect());
                    (BaseExpr::LetDecl(id, base > 0, Box::new(val)), TyName::Unit)
                }
                Rule::assignment => {
                    let composition = inner.into_inner().collect::<RuleList>();
                    let id = composition[0].as_str().to_string();
                    let val = walk_value_expr(composition[1].clone().into_inner().collect());
                    (BaseExpr::Assign(id, Box::new(val)), TyName::Unit)
                }
                Rule::value_expr => {
                    walk_value_expr(inner.into_inner().collect())
                }
                _ => {
                    (BaseExpr::Nope, TyName::Unit)
                }
            };
        } else {
            (BaseExpr::Nope, TyName::Unit)
        }
    }
    ).collect()
}

fn walk_value_expr(body: RuleList) -> TypedExpr {
    let mut priority = HashMap::<&str, (i32, bool)>::new();
    priority.insert("+", (1, true));
    priority.insert("-", (1, true));
    priority.insert("*", (2, true));
    priority.insert("/", (2, true));
    walk_value_expr_with_climber(body, 0, &priority).0
}

fn walk_value_expr_with_climber<'a>(body: RuleList<'a>, prec_val: i32, priority: &HashMap<&str, (i32, bool)>) -> (TypedExpr, Option<RuleList<'a>>) {
    let primary = body[0].clone();
    match primary.as_rule() {
        Rule::value => {
            (walk_value_node(primary.into_inner().collect()), None)
        }
        Rule::func_call => {
            let composition = primary.into_inner().collect::<RuleList>();
            let id = composition[0].as_str().to_string();
            ((BaseExpr::FuncCall(id.clone(), composition.into_iter().skip(1)
                .map(|v| walk_value_expr(v.into_inner().collect())).collect())
              , TyName::VarBind(format!("ret@{}", id))), None)
        }
        Rule::bin_op => { // prec climber
            let composition = primary.into_inner().collect::<RuleList>();
            let lhs = walk_value_node(composition[0].clone().into_inner().collect());
            let mut last_op = composition[1].clone();
            let mut op = last_op.as_str().to_string();
            let mut prior = priority.get(&op[..]).expect("Operator not found !");
            let mut result = lhs;
            let mut remnants: RuleList = composition[2].clone().into_inner().collect();
            while prior.0 >= prec_val {
                println!("{} {} {:?}", prec_val, op, prior);
                dbg!(&remnants);
                let next_prior = if prior.1 { prior.0 + 1 } else { prior.0 };
                let rhs = walk_value_expr_with_climber(remnants.clone(), next_prior, priority);
                let suc = rhs.1;
                result = gen_bin_op(&op, result, rhs.0);
                match suc {
                    None => {
                        return (result, None);
                    }
                    Some(expr) => {
                        dbg!(&expr);
                        remnants = expr;
                        last_op = remnants[0].clone();
                        op = last_op.as_str().to_string();
                        prior = priority.get(&op[..]).expect("Operator not found !");
                        remnants = remnants[1..].to_vec();
                    }
                }
            }
            //println!("remnants :");
            //dbg!(&remnants);
            if remnants.len() > 0 {
                let mut ret_vec = vec![last_op];
                ret_vec.extend(remnants.into_iter());
                (result, Some(ret_vec))
            } else {
                (result, None)
            }
        }
        _ => { ((BaseExpr::Nope, TyName::Unit), None) }
    }
}

fn walk_value_node(body: RuleList) -> TypedExpr {
    let inner = body[0].clone();
    if inner.as_rule() == Rule::ident {
        return (BaseExpr::Ident(inner.as_str().to_string()), TyName::VarBind(inner.as_str().to_string()));
    } else if inner.as_rule() == Rule::number {
        return (BaseExpr::ConstantInt(inner.as_str().to_string().parse().unwrap()), TyName::NameBind(String::from("i64")));
    }
    (BaseExpr::Nope, TyName::Unit)
}

fn walk_ty(ty: Pair<Rule>) -> TyName {
    if ty.as_rule() == Rule::ident {
        return TyName::NameBind(ty.as_span().as_str().to_string());
    } else if ty.as_rule() == Rule::mut_type {
        return TyName::MutBind(Box::new(walk_ty(ty.into_inner().next().unwrap())));
    } else if ty.as_rule() == Rule::arr_type {
        return TyName::Array(Box::new(walk_ty(ty.into_inner().next().unwrap())));
    } else if ty.as_rule() == Rule::type_ident || ty.as_rule() == Rule::immut_type || ty.as_rule() == Rule::ret_type {
        return walk_ty(ty.into_inner().next().unwrap());
    } else {
        return TyName::Unit;
    }
}