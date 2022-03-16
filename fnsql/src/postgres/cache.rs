use std::{borrow::Cow, collections::HashMap};
use postgres::{Statement, types::Type, Error, GenericClient};

type Key = (Cow<'static, str>, Cow<'static, [Type]>);

pub struct Cache {
    map: HashMap<Key, Statement>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn prepare(&mut self, query: &str, client: &mut impl GenericClient) -> Result<Statement, Error> {
        self.prepare_typed(query, &[], client)
    }

    pub fn prepare_typed(&mut self, query: &str, types: &[Type], client: &mut impl GenericClient) -> Result<Statement, Error> {
        let cow_types = Cow::Borrowed(types);
        let cow_query = Cow::Borrowed(query);

        match self.map.get(&(cow_query, cow_types)) {
            Some(stmt) => return Ok(stmt.clone()),
            None => {
                let stmt = client.prepare_typed(query, types)?;
                self.map.insert((Cow::Owned(query.to_owned()),
                    Cow::Owned(Vec::from(types))), stmt.clone());
                Ok(stmt)
            }
        }
    }
}
