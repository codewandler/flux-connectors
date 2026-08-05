op babelforce-delete-queue(id: String) -> Any
  description "Delete a queue"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{id}")
  response = http.request(method: "DELETE", url)
  return response
