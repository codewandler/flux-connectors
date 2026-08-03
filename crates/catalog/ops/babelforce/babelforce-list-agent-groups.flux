op babelforce-list-agent-groups(page: Number, max: Number) -> Any
  description "List agent groups"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/groups")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
