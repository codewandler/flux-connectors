op babelforce-list-prompt-files -> Any
  description "List prompt files"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/prompts")
  response = http.request(method: "GET", url)
  return response
