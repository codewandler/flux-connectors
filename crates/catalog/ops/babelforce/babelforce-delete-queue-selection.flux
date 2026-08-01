op babelforce-delete-queue-selection(queueId: String, id: String) -> Any
  description "Delete a queue selection"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections/{id}")
  response = http.request(method: "DELETE", url)
  return response
