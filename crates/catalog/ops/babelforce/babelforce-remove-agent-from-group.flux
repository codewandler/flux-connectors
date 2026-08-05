op babelforce-remove-agent-from-group(groupId: String, agentId: String) -> Any
  description "Remove an agent from a group"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/groups/{groupId}/agents/{agentId}")
  response = http.request(method: "DELETE", url)
  return response
