op babelforce-bulk-delete-event-triggers(body: Any) -> Any
  description "Bulk-delete event triggers"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers/bulk")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "DELETE", url)
  return response
