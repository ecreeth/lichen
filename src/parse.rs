use rand::random;
use std::collections::HashMap;

use eval::Eval;

#[derive(Debug,PartialEq)]
pub struct SrcBlock {
    pub name: String,
    pub src: Vec<Src>
}

#[derive(Debug,PartialEq)]
pub struct DefBlock {
    pub name: String,
    pub defs: Vec<(String,Var)>
}

#[derive(Debug,PartialEq)]
pub enum Block {
    Src(SrcBlock),
    Def(DefBlock),
}


/// delimited by new line
#[derive(Debug,PartialEq)]
pub enum Src {
    Logic(String, Logic), // ex: item_logic has_item

    // references logic in env and emits varkinds;
    // logic must resolve to true
    // ex: if item_logic give_quest
    // Can optionally end execution and begin next node
    If(Expect, Vec<Var>, Option<String>),

    Emit(Vec<Var>), //just emits variables
    
    Composite(String,Expect,Vec<String>),
    Next(String), // ends execution and begins next node
}

#[derive(Debug,PartialEq)]
pub enum Expect {
    All,
    Any,
    None,
    
    Ref(String) // references env variable set from logic
}
impl Expect {
    pub fn parse(s: String) -> Expect {
        match &s[..] {
            "all" => Expect::All,
            "any" => Expect::Any,
            "none" => Expect::None,
            _ => Expect::Ref(s),
        }
    }
}

impl Src {
    pub fn eval<D:Eval> (&self, state: &mut HashMap<String,bool>, data: &D)
                     -> (Vec<Var>,Option<String>)
    {
        match self {
            &Src::Next(ref node) => {
                return (vec![],Some(node.clone()))
            },
            &Src::Emit(ref vars) => {
                return (vars.clone(),None)
            },
            &Src::Logic(ref name, ref logic) => { //logic updates state
                let name = name.clone();
                match logic {
                    &Logic::Is(ref lookup) => {
                        let r = data.eval(&lookup);
                        if r.is_some() {
                            match r.unwrap() {
                                Var::Bool(v) => { state.insert(name,v); },
                                _ => { state.insert(name,true); }, //if exists?
                            }
                        }
                    },
                    &Logic::IsNot(ref lookup) => { //inverse state
                        let r = data.eval(&lookup);
                        if r.is_some() {
                            match r.unwrap() {
                                Var::Bool(v) => {
                                    if !v { state.insert(name,true); }
                                },
                                _ => { state.insert(name,false); },
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
                let mut comp_value = false;
                match x {
                    &Expect::All => { // all must pass as true
                        for lookup in lookups.iter() {
                            let val = state.get(lookup);
                            if val.is_some() && *val.unwrap() {
                                comp_value = true;
                            }
                            else { comp_value = false; break }
                        }
                        
                        state.insert(name.clone(),comp_value);
                    },
                    &Expect::Any => { // first truth passes for set
                        for lookup in lookups.iter() {
                            let val = state.get(lookup);
                            if val.is_some() && *val.unwrap() {
                                comp_value = true;
                                break;
                            }
                        }

                        state.insert(name.clone(),comp_value);
                    },
                    &Expect::None => { // inverse of any, none must be true
                        for lookup in lookups.iter() {
                            let val = state.get(lookup);
                            if val.is_some() && *val.unwrap() {
                                comp_value = false;
                                break;
                            }
                        }

                        state.insert(name.clone(),comp_value);
                    },
                    &Expect::Ref(_) => panic!("ERROR: Unexpected parsing") // this should never hit
                }

                return (vec![],None) // composite does not return anything
            },
            &Src::If(ref x, ref v, ref node) => {
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
                        let val = state.get(lookup);
                        if let Some(val) = val {
                            if_value = *val;
                        }
                    },
                }

                if if_value { return ((*v).clone(),node.clone()) }
                else { return (vec![],None) }
            }
        }
    }
    
    pub fn parse(mut exp: Vec<String>) -> Src {
        if exp[0] == "if" {
            if exp.len() < 3 { panic!("ERROR: Invalid IF Logic {:?}",exp) }
            
            let x = exp.remove(1);

            let mut node = None;
            if exp.len() > 2 {
                let next = &exp[exp.len() - 2] == "next";
                if next {
                    node = exp.pop();
                    let _ = exp.pop(); // remove next tag
                }
            }
            
            let v = exp.drain(1..).map(|n| Var::parse(n)).collect();
            Src::If(Expect::parse(x),
                        v, node)
        }
        else if exp[0] == "next" {
            if exp.len() == 2 {
                Src::Next(exp.pop().unwrap())
            }
            else { panic!("ERROR: Uneven NEXT Logic {:?}",exp) }
        }
        else if exp[0] == "emit" {
            if exp.len() > 1 {
                let mut v = vec![];
                for e in exp.drain(1..) {
                    v.push(Var::parse(e));
                }

                Src::Emit(v)
            }
            else { panic!("ERROR: Missing EMIT Logic {:?}",exp) }
        }
        else {
            let keys = exp.remove(0);
            let mut keys: Vec<&str> = keys.split_terminator(':').collect();

            if keys.len() < 2 { // regular logic
                Src::Logic(keys.pop().unwrap().to_owned(),
                               Logic::parse(exp))
            }
            else { // composite type
                let kind = Expect::parse(keys.pop().unwrap().to_owned());
                match kind { // only formal expected types allowed
                    Expect::Ref(_) => { panic!("ERROR: Informal Expect found {:?}", kind) },
                    _ => {}
                }
                Src::Composite(keys.pop().unwrap().to_owned(),
                                   kind,
                                   exp)
            }
        }
    }
}

/// delimited by new line
/// should resolve to boolean
#[derive(Debug,PartialEq)]
pub enum Logic {
    GT(Var,Var), // weight > 1
    LT(Var,Var),

    //boolean checks
    Is(String),
    IsNot(String),
}

impl Logic {
    pub fn parse(mut exp: Vec<String>) -> Logic {
        let len = exp.len();
        
        if len == 1 {
            let mut exp = exp.pop().unwrap();
            let inv = exp.remove(0);
            if inv == '!' {
                Logic::IsNot(exp)
            }
            else {
                exp.insert(0,inv);
                Logic::Is(exp)
            }
        }
        else if len == 3 {
            let var = exp.pop().unwrap();
            let var = Var::parse(var);

            let sym = exp.pop().unwrap();
            let key = exp.pop().unwrap();
            let key = Var::parse(key);
            
            if sym == ">" {
                Logic::GT(key,var)
            }
            else if sym == "<" {
                Logic::LT(key,var)
            }
            else { panic!("ERROR: Invalid Logic Syntax") }
        }
        else { panic!("ERROR: Unbalanced Logic Syntax ({:?})",exp) }
    }
}

#[derive(Debug,PartialEq, Clone)]
pub enum Var {
    String(String),
    Num(f32),
    Bool(bool),
}

impl ToString for Var {
    fn to_string(&self) -> String {
        match self {
            &Var::String(ref s) => s.clone(),
            &Var::Num(ref n) => n.to_string(),
            &Var::Bool(ref b) => b.to_string(),
        }
    }
}

impl From<bool> for Var {
    fn from(t:bool) -> Var {
        Var::Bool(t)
    }
}
impl From<f32> for Var {
    fn from(t:f32) -> Var {
        Var::Num(t)
    }
}
impl From<String> for Var {
    fn from(t:String) -> Var {
        Var::String(t)
    }
}
impl<'a> From<&'a str> for Var {
    fn from(t:&str) -> Var {
        Var::String(t.to_owned())
    }
}

impl Var {
    pub fn parse(t: String) -> Var {
        let val;

        if let Ok(v) = t.parse::<f32>() {
            val = Var::Num(v);
        }
        else if let Ok(v) = t.parse::<bool>() {
            val = Var::Bool(v);
        }
        else { val = Var::String(t) }
        
        val
    }

    pub fn get_num<D:Eval> (&self, data: &D) -> Result<f32,&'static str> {
        let num;
        match self {
            &Var::Num(n) => { num = n; },
            &Var::String(ref s) => {
                if let Some(n) = data.eval(s) {
                    match n {
                        Var::Num(n) => { num = n; },
                        _ => return Err("ERROR: NaN Evaluation")
                    }
                }
                else {  return Err("ERROR: Empty Evaluation") }
            },
            _ =>  return Err("ERROR: NaN Evaluation")
        }

        return Ok(num)
    }
}

pub struct Parser(Vec<Block>);

use std::ops::Deref;
impl Deref for Parser {
    type Target = Vec<Block>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Parser {
    pub fn parse_blocks (src: &str) -> Parser {
        let mut v = vec!();
        let mut exp = String::new();
        let mut exps: Vec<String> = vec!();
        let mut block: Option<Block> = None;

        let mut in_string = false;
        let mut in_comment = false;
        let mut in_vec = false;

        for c in src.chars() {
            if c == '[' { in_vec = true; continue }
            else if c == ']' { in_vec = false; }
            else if c == '#' && !in_string { in_comment = true; }
            else if  c == '\n' && in_comment && !in_string {
                in_comment = false;
                continue;
            }

            if c == '\n' && in_vec { continue }
            
            if (c == ']' ||
                c == '#' ||
                c == '\n')
                && !in_string
            {
                for n in exp.split_whitespace() {
                    exps.push(n.trim().to_owned());
                }
                exp = String::new();

                if exps.len() < 1 { continue }
                
                
                // determine block type
                if block.is_none() {
                    let name = exps.pop().unwrap();
                    
                    if name == "def" {
                        let b = DefBlock {
                            name: exps.pop().unwrap(),
                            defs: vec!()
                        };
                        
                        block = Some(Block::Def(b));
                    }
                    else {
                        let b = SrcBlock {
                            name: name,
                            src: vec!()
                        };
                        
                        block = Some(Block::Src(b));
                    }
                }
                else { // build block type
                    let mut qsyms = vec!();
                    for n in exps.iter_mut() {
                        if n.chars().next().expect("ERROR: Empty QSYM") == '\'' {
                            let mut qsym = "__".to_owned();
                            let sym = n[1..].trim().to_owned();
                            qsym.push_str(&random::<u16>().to_string());
                            
                            qsyms.push(qsym.clone());
                            qsyms.push(sym);
                            *n = qsym;
                        }
                    }
                    
                    match block {
                        Some(Block::Def(ref mut b)) => {
                            b.defs.push((exps[0].to_owned(),
                                         Var::parse(exps[1].to_owned())));
                        },
                        Some(Block::Src(ref mut b)) => {
                            //println!("EXPS{:?}",exps); //DEBUG
                            if qsyms.len() > 1 {
                                b.src.push(Src::parse(qsyms));
                            }
                            
                            b.src.push(Src::parse(exps));
                        },
                        _ => {}
                    }

                    exps = vec!();
                }
            }
            else if c == '"' && !in_comment {
                in_string = !in_string;
                if in_string {
                    for n in exp.split_whitespace() {
                        exps.push(n.trim().to_owned());
                    }
                    exp = String::new();
                }
                else if !in_string {
                    exps.push(exp);
                    exp = String::new();
                }
            }
            else if c == ';' && !in_string && !in_comment {
                //fail otherwise, block should be built!
                v.push(block.unwrap());
                block = None;
            }
            else {
                if !in_comment {
                    exp.push(c);
                }
            }
        }
        
        Parser(v)
    }

    pub fn into_env (mut self) -> Env {
        let mut src = HashMap::new();
        let mut def = HashMap::new();
        
        for b in self.0.drain(..) {
            match b {
                Block::Def(db) => {
                    def.insert(db.name.clone(), db);
                },
                Block::Src(sb) => {
                    src.insert(sb.name.clone(), sb);
                },
            }

            
        }

        Env { def: def, src: src }
    }
}

pub struct Env {
    pub def: HashMap<String, DefBlock>,
    pub src: HashMap<String, SrcBlock>
}
