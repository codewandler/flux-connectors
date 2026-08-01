op babelforce-get-prompt-uses(id: String) -> Any
  description "List a prompt's references"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/prompts/{id}/uses")
  response = http.request(method: "GET", url)
  return response
