op babelforce-delete-prompt(id: String) -> Any
  description "Delete a prompt"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/prompts/{id}")
  response = http.request(method: "DELETE", url)
  return response
