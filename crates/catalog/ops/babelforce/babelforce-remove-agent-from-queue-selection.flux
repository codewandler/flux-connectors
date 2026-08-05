op babelforce-remove-agent-from-queue-selection(queueId: String, selectionId: String, id: String) -> Any
  description "Remove an agent from a selection"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections/{selectionId}/agents/{id}")
  response = http.request(method: "DELETE", url)
  return response
