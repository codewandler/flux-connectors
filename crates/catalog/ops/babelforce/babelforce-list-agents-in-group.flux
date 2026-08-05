op babelforce-list-agents-in-group(groupId: String, page: Number, max: Number) -> Any
  description "List a group's agents"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/groups/{groupId}/agents")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
