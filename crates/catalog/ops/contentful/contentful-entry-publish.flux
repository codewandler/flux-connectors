op contentful-entry-publish(entry_id: String, version: Number) -> Any
  description "Publish a draft entry, making it visible through the Delivery API. Sends no body"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.contentful.com/spaces/{space_id}/environments/{environment_id}"
  url = fmt("{base}/entries/{entry_id}/published")
  response = http.request(headers: { "X-Contentful-Version": version }, method: "PUT", url)
  return response
