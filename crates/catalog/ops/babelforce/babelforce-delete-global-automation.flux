op babelforce-delete-global-automation(id: String) -> Any
  description "Delete an event trigger"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers/{id}")
  response = http.request(method: "DELETE", url)
  return response
