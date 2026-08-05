op babelforce-add-agent-to-queue-selection(queueId: String, selectionId: String, body: Any) -> Any
  description "Add an agent to a selection"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections/{selectionId}/agents")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
