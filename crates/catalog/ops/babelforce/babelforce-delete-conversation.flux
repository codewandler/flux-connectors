op babelforce-delete-conversation(id: String) -> Any
  description "Delete a conversation"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations/{id}")
  response = http.request(method: "DELETE", url)
  return response
