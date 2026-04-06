INSERT INTO import_indexer_result_media_domain (
    import_indexer_result_id,
    media_domain_id
)
SELECT $1, media_domain_id
FROM media_domain
WHERE media_domain_key::TEXT IN ('tv', 'movies')
