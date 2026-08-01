op babelforce-get-agent-import-job(id: String) -> Any
  description "Get an agent-import job"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/provision/jobs/{id}")
  response = http.request(method: "GET", url)
  return response
