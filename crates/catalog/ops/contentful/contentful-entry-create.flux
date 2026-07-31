op contentful-entry-create(content_type_id: String, body: Any) -> Any
  description "Create a new entry from a content type's field values. Contentful assigns the entry's id. The entry is created as a draft — it is not visible through the Delivery API until contentful-entry-publish publishes it"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.contentful.com/spaces/{space_id}/environments/{environment_id}"
  url = fmt("{base}/entries")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "X-Contentful-Content-Type": content_type_id, "content-type": content_type }, method: "POST", url)
  return response
