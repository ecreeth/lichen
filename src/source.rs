use std::collections::HashMap;

use ::{Logic,Expect};
use eval::Eval;
use var::{Var,Mut};
use parse::{Parser,Map,Def,IR};

/// Source block statement types
#[derive(Debug,PartialEq)]
pub enum Src {
    /// Standard logic, eg: has_sword_item i
    Logic(String, Logic),

    /// References logic in env and emits varkinds
    ///
    /// Logic must resolve to true
    /// eg: if item_logic give_quest
    /// Can optionally end execution and begin next node
    If(Expect, Vec<Var>, Option<Next>),

    /// Or must follow an previous If
    ///
    /// Or only fires when previous If logic fails
    Or(Vec<Var>,Option<Next>),

    /// Just emits variables
    Emit(Vec<Var>), 

    /// A composite logic type to group logic statements together
    Composite(String,Expect,Vec<String>),

    /// Ends execution and begins next node
    Next(Next),

    /// Mutate type, var being mutated, argument vars
    Mut(Mut, String, Vec<Var>),

    /// Match-like behavior for Mutations
    ///
    /// Map format should have Logic-Tested for the key
    /// and Mutation Function Signature for the value
    When(WhenMap),
}

/// Internal type to hold a specialized When-Mutate Map
pub type WhenMap = HashMap<String,(Mut,String,Vec<Var>)>;

/// Next-node action types
#[derive(Debug,PartialEq,Clone)]
pub enum Next {
    /// Instantly advances
    Now(String),

    /// Restarts current node
    Restart,

    /// Heads back to previous node visited
    Back,

    /// Awaits for manual advancement, failure to advance continues current node
    Await(String),

    /// Select from a group, based on decision
    Select(Map),
}
impl Next {
    pub fn parse(exp: &mut Vec<IR>) -> Result<Next,&'static str> {
        let mut select_idx = None;
        for (i,n) in exp.iter().enumerate() {
            match n {
                &IR::Sym(ref s) => {
                    if s == &"next:select" {
                        select_idx = Some(i);
                        break
                    }
                },
                _ => {},
            }
        }
        
        
        // handle nested selects as a special case
        if let Some(idx) = select_idx {
            let map_ir = exp.remove(idx+1);
            let _ = exp.remove(idx); // next:select statement
            if let Ok(map) = Parser::parse_map(map_ir) {
                return Ok(Next::Select(map))
            }
            else { return Err("Cannot parse map") }
        }
        

        let next;
        if let Some(node) = exp.pop() {
            if let Some(tag) = exp.pop() {
                match tag {
                    IR::Sym(tag) => {
                        let mut next_tag = tag.split_terminator(':');
                        let is_next = next_tag.next() == Some("next");
                        
                        if is_next {
                            let next_tag = next_tag.next();
                            match next_tag {
                                Some("now") => { next = Next::Now(node.into()) },
                                Some("await") => { next = Next::Await(node.into()) },
                                _ => { return Err("Invalid Next Type Found") },
                            }
                        }
                        else if next_tag.next().is_some() {
                            return Err("Unknown Tag encountered")
                        }
                        else {
                            exp.push(IR::Sym(tag.to_owned()));
                            exp.push(node);
                            return Err("Invalid Tag type")
                        }
                    },
                    _ => {
                        exp.push(tag);
                        exp.push(node);
                        return Err("Invalid Tag type")
                    }
                }
            }
            else {
                match node {
                    IR::Sym(ref tag) => {
                        let tag: &str = &tag;
                        match tag {
                            "next:back" => { next = Next::Back },
                            "next:restart" => { next = Next::Restart },
                            _ => {
                                exp.push(IR::Sym(tag.to_owned()));
                                return Err("Invalid Tag type")
                            },
                        }
                    },
                    _ => {
                        exp.push(node);
                        return Err("Missing Tag type")
                    }
                }
            }
        }
        else { return Err("No Next type found") }

        Ok(next)
    }
}


impl Src {
    pub fn eval<D:Eval> (&self, state: &mut HashMap<String,bool>,
                         data: &mut D,
                         def: &mut Def)
                         -> (Vec<Var>,Option<Next>)
    {
        match self {
            &Src::Mut(ref m, ref v, ref a) => {
                match m {
                    &Mut::Add | &Mut::Sub | &Mut::Mul | &Mut::Div => {
                        let mut num = None;

                        let var_name = Var::Sym(v.to_owned());
                        let mut v1 = Var::get_num(&var_name, def);
                        let is_def = v1.is_ok();
                        if !is_def { v1 = Var::get_num(&var_name, data); }
                        
                        if let Ok(v1) = v1 {
                            let var_name = &a[0];
                            let mut v2 = Var::get_num(&var_name, def);
                            if v2.is_err() { v2 = Var::get_num(&var_name, data); }
                            
                            if let Ok(v2) = v2 {
                                match m {
                                    &Mut::Add => {
                                        num = Some(v1+v2);
                                    },
                                    &Mut::Sub => {
                                        num = Some(v1-v2);
                                    },
                                    &Mut::Mul => {
                                        num = Some(v1*v2);
                                    },
                                    &Mut::Div => {
                                        num = Some(v1/v2);
                                    },
                                    _ => {},
                                }
                            }
                        }
                        
                        if let Some(num) = num {
                            if is_def { def.set_path(&v, Var::Num(num)); }
                            else { data.set_path(&v, Var::Num(num)); }
                        }
                    },
                    &Mut::Swap => {
                        let val = a[0].clone();
                        if def.get_path(v).is_some() {
                            def.set_path(v,val);
                        }
                        else { data.set_path(&v, val); }
                    },
                    &Mut::Fn(ref fun) => {
                        let mut args = vec![]; //collect symbols' value
                        for n in a {
                            match n {
                                &Var::Sym(ref n) => {
                                    if let Some(var) = def.get_path(n) {
                                        args.push(var);
                                    }
                                    else {
                                        if let Some(var) = data.get_path(n) {
                                            args.push(var);
                                        }
                                    }
                                },
                                _ => { args.push(n.clone()) }
                            }
                        }

                        
                        if let Some(var) = def.get_path(v) {
                            if let Some(r) = data.call(var, fun, &args) {
                                def.set_path(&v, r);
                            }
                        }
                        else if let Some(var) = data.get_path(&v) {
                            if let Some(r) = data.call(var, fun, &args) {
                                data.set_path(&v, r);
                            }
                        }
                    },
                }
                
                return (vec![],None)
            }
            &Src::Next(ref next) => {
                return (vec![],Some(next.clone()))
            },
            &Src::Or(ref vars, ref next) => {
                return (vars.clone(), next.clone())
            },
            &Src::Emit(ref vars) => {
                return (vars.clone(),None)
            },
            &Src::Logic(ref name, ref logic) => { //logic updates state
                let name = name.clone();
                match logic {
                    &Logic::Is(ref lookup) => {
                        if let Some(r) = data.get_path(&lookup) {
                            match r {
                                Var::Bool(v) => { state.insert(name,v); },
                                _ => { state.insert(name,true); }, //if exists?
                            }
                        }
                        else { //check state table: some_thing !some_otherthing
                            let mut val = None;
                            if let Some(r) = state.get(lookup) {
                                val = Some(*r);
                            }

                            if let Some(val) = val {
                                state.insert(name,val);
                            }
                        }
                    },
                    &Logic::IsNot(ref lookup) => { //inverse state
                        let r = data.get_path(&lookup);
                        
                        if r.is_some() {
                            match r.unwrap() {
                                Var::Bool(v) => {
                                    if !v { state.insert(name,true); }
                                },
                                _ => { state.insert(name,false); },
                            }
                        }
                        else {
                            let mut val = None;
                            if let Some(r) = state.get(lookup) {
                                val = Some(!r);
                            }

                            if let Some(val) = val {
                                state.insert(name,val);
                            }
                        }
                    },

                    &Logic::GT(ref left, ref right) => {
                        let right = Var::get_num::<D>(right,data);
                        let left = Var::get_num::<D>(left,data);
                        
                        if left.is_ok() && right.is_ok() {
                            state.insert(name, left.unwrap() > right.unwrap());
                        }
                    },
                    &Logic::LT(ref left, ref right) => {
                        let right = Var::get_num::<D>(right,data);
                        let left = Var::get_num::<D>(left,data);
                        
                        if left.is_ok() && right.is_ok() {
                            state.insert(name, left.unwrap() < right.unwrap());
                        }
                    },
                }

                return (vec![],None) // logic does not return anything
            },
            &Src::Composite(ref name, ref x, ref lookups) => {
                // track if any lookups are false or true
                let mut comp_false = false;
                let mut comp_true = false;
                
                for lookup in lookups.iter() {
                    let val = state.get(lookup);
                    if val.is_some() && *val.unwrap() {
                        comp_true = true;
                    }
                    else {
                        if val.is_some() { //found it but it's false
                            comp_false = true;
                        }
                        else { //check data for delayed reference
                            if let Some(val) = data.get_path(lookup) {
                                match val {
                                    Var::Bool(b) => {
                                        if b { comp_true = true; }
                                        else { comp_false = true; }
                                    }
                                    _ => { comp_true = true; } //identity/exists, true
                                }
                            }
                        }
                    }
                }
                
                match x {
                    &Expect::All => { // all must pass as true
                        if comp_true && !comp_false {
                            state.insert(name.clone(),true);
                        }
                    },
                    &Expect::Any => { // first truth passes for set
                        if comp_true {
                            state.insert(name.clone(),true);
                        }
                    },
                    &Expect::None => { // inverse of any, none must be true
                        if !comp_true && comp_false {
                            state.insert(name.clone(),true);
                        }
                    },
                    &Expect::Ref(_) => {} // this should never hit
                }

                return (vec![],None) // composite does not return anything
            },
            &Src::If(ref x, ref v, ref next) => {
                let mut if_value = false;
                match x {
                    &Expect::All => {
                        for n in state.values() {
                            if !n { if_value = false; break }
                            else { if_value = true; }
                        }
                    },
                    &Expect::Any => {
                        for n in state.values() {
                            if *n { if_value = true; break }
                        }
                    },
                    &Expect::None => {
                        for n in state.values() {
                            if !n { if_value = true; }
                            else { if_value = true; break }
                        }
                    },
                    &Expect::Ref(ref lookup) => {
                        let has_val = {
                            let val = state.get(lookup);
                            if let Some(val) = val {
                                if_value = *val;
                            }

                            val.is_some()
                        };

                        if !has_val {
                            if let Some(val) = data.get_path(lookup) {
                                match val {
                                    Var::Bool(v) => { if_value = v; },
                                    _ => { if_value = true; }
                                }
                            }
                        }
                    },
                }

                if if_value { return ((*v).clone(), next.clone()) }
                else { return (vec![],None) }
            },
            &Src::When(ref map) => {
                for (k, &(ref m, ref v, ref a)) in map.iter() {
                    let mut is_true = false;
                    if let Some(b) = state.get(k) { is_true = *b }
                    if is_true {
                        Src::eval(&Src::Mut(m.clone(), v.clone(), a.clone()), state, data, def);
                    }
                }
                
                return (vec![],None)
            },
        }
    }
    
    pub fn parse(mut exp: Vec<IR>) -> Result<Src,&'static str> {
        let ir = exp.remove(0);
        match ir {
            IR::Sym(ref sym) => {
                if sym.chars().next() == Some('@') { //mutating statement
                    exp.insert(0,IR::Sym(sym.to_owned()));
                    let (m, v, a) = try!(Mut::parse(&mut exp));
                    return Ok(Src::Mut(m,v,a))
                }
                else if sym == "when" {
                    if exp.len() != 1 { return Err("Invalid WHEN Logic") }
                    if let Ok(mut map) = Parser::parse_map(exp.pop().unwrap()) {
                        let mut when_map: WhenMap = HashMap::new();
                        for (k,mut v) in map.drain() {
                            let v_ir = v.drain(..).map(|n| n.into()).collect();
                            let m = try!(Src::parse(v_ir));
                            match m {
                                Src::Mut(m,v,a) => {
                                    when_map.insert(k, (m,v,a));
                                },
                                _ => { return Err("Invalid WHEN Logic"); }
                            }
                        }

                        if when_map.is_empty() { return Err("Unable to parse WHEN Map into Mut") }
                        Ok(Src::When(when_map))
                    }
                    else { Err(" Invalid WHEN Logic") }
                }
                else if sym == "if" {
                    if exp.len() < 2 { return Err("Invalid IF Logic") }

                    let x = exp.remove(0);
                    let next = Next::parse(&mut exp);
                    
                    let mut v = vec![];
                    for n in exp.drain(..) {
                        let r = try!(Var::parse(n));
                        v.push(r);
                    }

                    Ok(Src::If(Expect::parse(x.into()),
                               v, next.ok()))
                }
                else if sym == "or" {
                    if exp.len() < 1 { return Err("Invalid OR Logic") }

                    let next = Next::parse(&mut exp);
                    
                    let mut v = vec![];
                    for n in exp.drain(..) {
                        let r = try!(Var::parse(n));
                        v.push(r);
                    }
                    
                    Ok(Src::Or(v,next.ok()))
                }
                else if &sym.split_terminator(':').next() == &Some("next") {
                    exp.insert(0, IR::Sym(sym.to_owned()));
                    let next = Next::parse(&mut exp);
                    if let Ok(next) = next {
                        Ok(Src::Next(next))
                    }
                    else { Err("Invalid NEXT Logic") }
                }
                else if sym == "emit" {
                    if exp.len() > 0 {
                        let mut v = vec![];
                        for e in exp.drain(..) {
                            let r = try!(Var::parse(e));
                            v.push(r);
                        }

                        Ok(Src::Emit(v))
                    }
                    else { Err("Missing EMIT Logic") }
                }
                else {
                    let mut keys: Vec<&str> = sym.split_terminator(':').collect();
                    if keys.len() < 2 { // regular logic
                        let r = try!(Logic::parse(exp));
                        Ok(Src::Logic(sym.to_owned(),
                                      r))
                    }
                    else { // composite type
                        let kind = Expect::parse(keys.pop().unwrap().to_owned());
                        match kind { // only formal expected types allowed
                            Expect::Ref(_) => { return Err("Informal Expect found") },
                            _ => {}
                        }

                        let exp = exp.drain(..).map(|n| n.into()).collect();
                        Ok(Src::Composite(keys.pop().unwrap().to_owned(),
                                          kind,
                                          exp))
                    }
                }
            },
            _ => { Err("Encountered Non-Symbol Token") },
        }
    }
}
