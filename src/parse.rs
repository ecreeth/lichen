#[derive(Debug,PartialEq)]
pub struct SrcBlock {
    pub name: String,
    pub src: Vec<SrcKind>
}

#[derive(Debug,PartialEq)]
pub struct DefBlock {
    pub name: String,
    pub defs: Vec<(String,VarKind)>
}

#[derive(Debug,PartialEq)]
pub enum BlockKind {
    Src(SrcBlock),
    Def(DefBlock),
}


/// delimited by new line
#[derive(Debug,PartialEq)]
pub enum SrcKind {
    Logic(String, LogicKind), // ex: item_logic has_item

    // references logic in env and either--
    // node destination or return varkind;
    // logic must resolve to true
    // ex: if item_logic give_quest
    If(String, String),

    Next(String),
    Return(VarKind),
}

impl SrcKind {
    pub fn parse(exp: Vec<&str>) -> SrcKind {
        if exp[0] == "if" {
            SrcKind::If(exp[1].to_owned(), exp[2].to_owned())
        }
        else if exp[0] == "next" {
            SrcKind::Next(exp[1].to_owned())
        }
        else if exp[0] == "return" {
            SrcKind::Return(VarKind::parse(exp[1]))
        }
        else {
            SrcKind::Logic(exp[0].to_owned(),
                           LogicKind::parse(exp))
        }
    }
}

/// delimited by new line
/// should resolve to boolean
#[derive(Debug,PartialEq)]
pub enum LogicKind {
    GT(String,f32), // weight > 1
    LT(String,f32),

    //boolean checks
    Is(String),
    IsNot(String),
}

impl LogicKind {
    pub fn parse(exp: Vec<&str>) -> LogicKind {
        let start = 1;
        let len = exp.len() - start;
        
        if len == 1 {
            if exp[start].split_at(1).0 == "!" {
                LogicKind::IsNot(exp[start][1..].to_owned())
            }
            else {
                LogicKind::Is(exp[start][1..].to_owned())
            }
        }
        else if len == 3 {
            let var = VarKind::parse(exp[start+2]);

            match var {
                VarKind::Num(num) => {
                    let key = exp[start].to_owned();
                    let sym = exp[start + 1];
                    
                    if sym == ">" {
                        LogicKind::GT(key,num)
                    }
                    else if sym == "<" {
                        LogicKind::LT(key,num)
                    }
                    else { panic!("ERROR: Invalid LogicKind Syntax") }
                },
                _ => { panic!("ERROR: Invalid LogicKind Value {:?}",exp) }
            }
        }
        else { panic!("ERROR: Unbalanced LogicKind Syntax ({:?})",exp) }
    }
}

#[derive(Debug,PartialEq)]
pub enum VarKind {
    String(String),
    Num(f32),
    Bool(bool),
}

impl VarKind {
    pub fn parse(t: &str) -> VarKind {
        let val;

        if let Ok(v) = t.parse::<f32>() {
            val = VarKind::Num(v);
        }
        else if let Ok(v) = t.parse::<bool>() {
            val = VarKind::Bool(v);
        }
        else { val = VarKind::String(t.to_owned()) }
        
        val
    }
}

pub struct Parser;
impl Parser {
    pub fn parse_blocks (src: &str) -> Vec<BlockKind> {
        let mut v = vec!();
        let mut exp = String::new();
        let mut block: Option<BlockKind> = None;
        let mut kind: Option<VarKind> = None;

        let mut in_string = false;

        for c in src.chars() {
            if c == '\n' && !in_string {
                let exp_ = exp;
                exp = String::new();
                let mut exp: Vec<&str> = exp_
                    .split_whitespace()
                    .map(|x| x.trim())
                    .collect();

                if exp.len() < 1 { continue }
                
                
                // determine block type
                if block.is_none() {
                    if exp[0] == "with" {
                        let b = DefBlock {
                            name: exp[1].to_owned(),
                            defs: vec!()
                        };
                        
                        block = Some(BlockKind::Def(b));
                    }
                    else {
                        let b = SrcBlock {
                            name: exp[0].to_owned(),
                            src: vec!()
                        };
                        
                        block = Some(BlockKind::Src(b));
                    }
                }
                else { // build block type
                    match block {
                        Some(BlockKind::Def(ref mut b)) => {
                            b.defs.push((exp[0].to_owned(),
                                        VarKind::parse(&exp[1])));
                        },
                        Some(BlockKind::Src(ref mut b)) => {
                            b.src.push(SrcKind::parse(exp));
                        },
                        _ => {}
                    }
                }
            }
            else if c == '"' {
                in_string = !in_string;
                if !in_string {}
            }
            else if c == ';' && !in_string {
                //fail otherwise, block should be built!
                v.push(block.unwrap());
                block = None;
            }
            else {
                exp.push(c);
            }
        }
        
        v
    }
}
