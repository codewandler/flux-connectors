op contentful-entry-get(entry_id: String) -> Any
  description "Get one published entry by id, with its fields resolved to one locale"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://cdn.contentful.com/spaces/{space_id}/environments/{environment_id}"
  url = fmt("{base}/entries/{entry_id}")
  response = http.request(method: "GET", url)
  return response
