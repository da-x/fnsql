fnsql::fnsql! {
    #[postgres, test]
    create_table_pet() {
        "CREATE TABLE pet (
              id      INTEGER PRIMARY KEY,
              name    TEXT NOT NULL,
              data    BYTEA
        )"
    }

    #[postgres, test(with=[create_table_pet])]
    get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
        "SELECT id, data FROM pet WHERE pet.name = $1"
    }

    #[postgres, named, test(with=[create_table_pet])]
    insert_new_pet(id: i32, name: String, data: Option<Vec<u8>>) {
        "INSERT INTO pet (id, name, data) VALUES (:id, :name, :data)"
    }

    #[postgres, test(with=[create_table_pet])]
    insert_new_pet_str(id: i32, name: str, data: Option<Vec<u8>>) {
        "INSERT INTO pet (id, name, data) VALUES ($1, $2, $3)"
    }

    #[postgres, test(with=[create_table_pet])]
    update_pet_data(name: str, data: [u8]) {
        "UPDATE pet SET data = $2 WHERE name = $1"
    }

    #[postgres, test(with=[create_table_pet])]
    get_pet_count(pet_id: i32) -> [(i32)] {r#"
         SELECT count(*)
           FROM pet
          WHERE id = $1
    "#}
}


#[derive(Debug)]
struct Pet {
    id: i32,
    name: String,
    data: Option<Vec<u8>>,
}

pub fn main() -> Result<(), postgres::Error> {
    let mut conn = fnsql::postgres::testing_client()?;
    conn.execute("SET search_path TO pg_temp", &[]).unwrap();
    conn.execute("CREATE TYPE foo AS ENUM ('Bar', 'Baz')", &[]).unwrap();

    conn.execute_create_table_pet()?;

    let mut me = Pet {
        id: 0,
        name: "Max".to_string(),
        data: None,
    };

    conn.execute_insert_new_pet(&me.id, &me.name, &me.data)?;

    me.id += 1;
    let prep = conn.prepare_insert_new_pet()?;
    conn.execute_prepared_insert_new_pet(&prep, &me.id, &me.name, &me.data)?;

    me.id += 1;
    let mut cache = fnsql::postgres::Cache::new();
    let prep = conn.prepare_cached_insert_new_pet(&mut cache)?;
    conn.execute_prepared_insert_new_pet(&prep, &me.id, &me.name, &me.data)?;

    Ok(())
}
