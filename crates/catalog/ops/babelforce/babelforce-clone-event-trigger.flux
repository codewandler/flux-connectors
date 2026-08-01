op babelforce-clone-event-trigger(id: String) -> Any
  description "Clone an event trigger"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers/{id}/clone")
  response = http.request(method: "POST", url)
  return response
