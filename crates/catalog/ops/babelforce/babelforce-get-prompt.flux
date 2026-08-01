op babelforce-get-prompt(id: String) -> Any
  description "Get a prompt"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/prompts/{id}")
  response = http.request(method: "GET", url)
  return response
