op babelforce-delete-sms(id: String) -> Any
  description "Delete an SMS"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sms/{id}")
  response = http.request(method: "DELETE", url)
  return response
