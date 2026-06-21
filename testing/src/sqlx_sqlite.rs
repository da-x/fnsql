fnsql::fnsql! {
    #[sqlx_sqlite, test]
    create_table_pet() {
        "CREATE TABLE pet (
              id      INTEGER PRIMARY KEY,
              name    TEXT NOT NULL,
              data    BLOB
        )"
    }

    #[sqlx_sqlite, test(with=[create_table_pet])]
    get_pet_id_data(name: Option<String>) -> [(i32, Option<Vec<u8>>)] {
        "SELECT id, data FROM pet WHERE pet.name = :name"
    }

    #[sqlx_sqlite, test(with=[create_table_pet])]
    insert_new_pet(name: String, data: Option<Vec<u8>>) {
        "INSERT INTO pet (name, data) VALUES (:name, :data)"
    }

    #[sqlx_sqlite, test(with=[create_table_pet])]
    get_pet_count(pet_id: i64) -> [(i64)] {
        "SELECT count(*) FROM pet WHERE id = :pet_id"
    }
}

pub async fn main() -> Result<(), sqlx::Error> {
    let pool = fnsql::sqlx::testing_pool().await?;

    pool.execute_create_table_pet().await?;

    let name = "Max".to_string();
    let data: Option<Vec<u8>> = None;
    pool.execute_insert_new_pet(&name, &data).await?;

    let rows = pool.query_get_pet_id_data(&Some("Max".to_string())).await?;
    for pet in rows {
        println!("Found pet ({:?}, {:?})", pet.0, pet.1);
    }

    let count = pool.query_one_get_pet_count(&0).await?;
    println!("Pet count for id=0: {}", count);

    let opt = pool.query_opt_get_pet_id_data(&Some("Nonexistent".to_string())).await?;
    println!("Opt is none: {}", opt.is_none());

    Ok(())
}
