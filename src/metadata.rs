use std::collections::HashMap;
use std::rc::Rc;

// https://protobuf.dev/programming-guides/proto3/

#[derive(Clone)]
pub enum Kind {
    Double,
    Float,
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Bool,
    String,
    Bytes,
    Message(Rc<Message>),
    Map(Rc<Message>),
}

#[derive(Clone)]
pub struct Message {
    name: String,
    fields: Vec<Field>,
    tags: Vec<isize>,
    field_names: Option<HashMap<String, usize>>,
}

#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub tag: u32,
    pub kind: Kind,
    pub repeated: bool,
}

impl Message {
    pub fn new(name: String, fields: Vec<Field>, field_map: bool) -> Self {
        let max_tag = fields.iter().fold(0, |a, f| a.max(f.tag)) as usize;
        let tags = if max_tag < fields.len() + fields.len() / 4 + 3 {
            let mut tags = vec![-1; max_tag + 1];
            for (i, f) in fields.iter().enumerate() {
                tags[f.tag as usize] = i as isize;
            }
            tags
        } else {
            let mut tags = (0..fields.len() as isize).collect::<Vec<_>>();
            tags.sort_by(|a, b| fields[*a as usize].tag.cmp(&fields[*b as usize].tag));
            tags
        };
        let field_names = field_map.then(|| {
            fields
                .iter()
                .enumerate()
                .map(|x| (x.1.name.clone(), x.0))
                .collect::<HashMap<_, _>>()
        });
        Self {
            name,
            fields,
            tags,
            field_names,
        }
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    pub fn get_fields(&self) -> &[Field] {
        self.fields.as_slice()
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Field> {
        if let Some(ref m) = self.field_names {
            m.get(name).map(|&idx| &self.fields[idx])
        } else {
            self.fields.iter().find(|f| f.name == name)
        }
    }

    pub fn get_by_tag(&self, tag: u32) -> Option<&Field> {
        if self.tags.len() == self.fields.len() {
            self.tags
                .binary_search_by(|&x| tag.cmp(&self.fields[x as usize].tag))
                .ok()
                .map(|x| &self.fields[self.tags[x] as usize])
        } else {
            self.tags.get(tag as usize).and_then(|&x| {
                if x >= 0 {
                    Some(&self.fields[x as usize])
                } else {
                    None
                }
            })
        }
    }
}
