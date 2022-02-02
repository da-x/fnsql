fnsql::fnsql! {
    #[rusqlite, test]
    create_table_pet() {
        "CREATE TABLE pet (
              id      INTEGER PRIMARY KEY,
              name    TEXT NOT NULL,
              data    BLOB
        )"
    }

    #[rusqlite, test(with=[create_table_pet])]
    get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
        "SELECT id, data FROM pet WHERE pet.name = :name"
    }

    #[rusqlite, test(with=[create_table_pet])]
    insert_new_pet(name: String, data: Option<Vec<u8>>) {
        "INSERT INTO pet (name, data) VALUES (:name, :data)"
    }

    #[rusqlite, test(with=[create_table_pet])]
    insert_new_pet_str(name: str, data: Option<Vec<u8>>) {
        "INSERT INTO pet (name, data) VALUES (:name, :data)"
    }
}

#[derive(Debug)]
struct Pet {
    _id: i32,
    name: String,
    data: Option<Vec<u8>>,
}

fn main() -> rusqlite::Result<()> {
    let mut conn = rusqlite::Connection::open_in_memory()?;

    {
        conn.execute_create_table_pet()?;
        let me = Pet {
            _id: 0,
            name: "Max".to_string(),
            data: None,
        };
        conn.execute_insert_new_pet(&me.name, &me.data)?;
        {
            let mut stmt = conn.prepare_get_pet_id_data()?;
            let pet_iter = stmt.query_map(&Some("Max".to_string()), |_id, data| {
                Ok::<_, rusqlite::Error>(Pet {
                    _id,
                    data,
                    name: "Max".to_string(),
                })
            })?;

            for pet in pet_iter {
                println!("Found pet {:?}", pet.unwrap());
            }

            for pet in stmt.query(&Some("Max".to_string()))? {
                let pet = pet?;
                println!("Found pet {:?}", pet);
            }

            {
                let mut rows = stmt.query(&Some("Max".to_string()))?;
                while let Some(Ok(pet)) = rows.next() {
                    println!("Found pet {:?}", pet);
                }
            }
        }
        {
            let mut stmt = conn.prepare_cached_get_pet_id_data()?;
            let _pet_iter = stmt.query_map(&Some("Max".to_string()), |_id, data| {
                Ok::<_, rusqlite::Error>(Pet {
                    _id,
                    data,
                    name: "Max".to_string(),
                })
            })?;
        }
        conn.execute_insert_new_pet_str(&me.name, &me.data)?;

        {
            let mut stmt = conn.prepare_insert_new_pet_str()?;
            stmt.execute(&me.name, &me.data)?;
        }

        {
            let mut stmt = conn.prepare_cached_insert_new_pet_str()?;
            stmt.execute(&me.name, &me.data)?;
        }

        let _pet: Pet = conn.query_row_get_pet_id_data(&Some("Max".to_string()), |_id, data| {
            Pet { _id, data, name: "Max".to_string() }
        })?;
    }

    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare_cached_get_pet_id_data()?;
        {
            let _pet_iter = stmt.query_map(&Some("Max".to_string()), |_id, data| {
                Ok::<_, rusqlite::Error>(Pet {
                    _id,
                    data,
                    name: "Max".to_string(),
                })
            })?;
        }
        let _pet: Pet = stmt.query_row(&Some("Max".to_string()), |_id, data| {
            Pet { _id, data, name: "Max".to_string() }
        })?;
    }

    tx.commit()?;

    Ok(())
}
