op babelforce-delete-custom-event(id: String) -> Any
  description "Delete a custom event"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/custom/{id}")
  response = http.request(method: "DELETE", url)
  return response
