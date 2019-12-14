DELETE FROM timeline WHERE id IN
	(
		SELECT timeline.id FROM timeline
		INNER JOIN timeline_definition ON timeline.timeline_id = timeline_definition.id
		WHERE timeline_definition.query LIKE 'followed or [%]' OR
			timeline_definition.query = 'local' OR
			timeline_definition.query = 'all'
	);
