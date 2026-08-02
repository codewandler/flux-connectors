op openai-batch-list(limit: Number) -> Any
  description "List batch jobs available to this API key with a bounded integer limit"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/batches")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
