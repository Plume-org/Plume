-- Your SQL goes here
--#!|conn: &Connection, path: &Path| {
--#!    let mut pb = path.to_path_buf();
--#!    pb.push("search_index");
--#!	let searcher = super::search::Searcher::create(&pb)?;
--#!	searcher.fill(conn)?;
--#!	searcher.commit();
--#!	Ok(())
--#!}

