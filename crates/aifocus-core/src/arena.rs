//! Allocation-stable arenas for compiler-owned structured data.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArenaId(pub u32);

#[derive(Debug, Clone, Default)]
pub struct Arena<T> {
    values: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
        }
    }

    pub fn alloc(&mut self, value: T) -> ArenaId {
        let id = ArenaId(self.values.len() as u32);
        self.values.push(value);
        id
    }

    pub fn get(&self, id: ArenaId) -> Option<&T> {
        self.values.get(id.0 as usize)
    }

    pub fn get_mut(&mut self, id: ArenaId) -> Option<&mut T> {
        self.values.get_mut(id.0 as usize)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ArenaId, &T)> {
        self.values
            .iter()
            .enumerate()
            .map(|(index, value)| (ArenaId(index as u32), value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSchema {
    pub name: String,
    pub fields: Vec<RecordField>,
}

impl RecordSchema {
    pub fn new(name: impl Into<String>, fields: Vec<RecordField>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordValue {
    pub schema: String,
    pub fields: Vec<(String, String)>,
}

impl RecordValue {
    pub fn new(schema: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            fields: Vec::new(),
        }
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        if let Some((_, existing)) = self.fields.iter_mut().find(|(field, _)| field == &name) {
            *existing = value.into();
        } else {
            self.fields.push((name, value.into()));
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_ids_are_stable() {
        let mut arena = Arena::with_capacity(2);
        let first = arena.alloc("root");
        let second = arena.alloc("child");
        assert_eq!(arena.get(first), Some(&"root"));
        assert_eq!(arena.get(second), Some(&"child"));
    }

    #[test]
    fn records_update_without_duplicate_fields() {
        let mut value = RecordValue::new("Token");
        value.set("kind", "Ident");
        value.set("kind", "Number");
        assert_eq!(value.get("kind"), Some("Number"));
    }
}
