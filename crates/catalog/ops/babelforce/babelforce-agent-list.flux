op babelforce-agent-list(page: Number, max: Number, q: String, enabled: Bool, name: String, number: String, sourceId: String, state: String, source: String, groupIds: String, groups: String, tags: String) -> Any
  description "List and filter agents. Doubles as the verification operation — cheap, read-only, and it fails loudly on a bad credential; this API has no /me endpoint"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents")
  response = http.request(method: "GET", query: { enabled, groupIds, groups, max, name, number, page, q, source, sourceId, state, tags }, url)
  return response
