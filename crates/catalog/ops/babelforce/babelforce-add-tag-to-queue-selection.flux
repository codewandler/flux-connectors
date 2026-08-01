op babelforce-add-tag-to-queue-selection(queueId: String, selectionId: String, body: Any) -> Any
  description "Add a tag to a selection"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections/{selectionId}/tags")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
