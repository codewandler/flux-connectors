op babelforce-delete-phonebook-entry(id: String) -> Any
  description "Delete a phonebook entry"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/phonebook/{id}")
  response = http.request(method: "DELETE", url)
  return response
