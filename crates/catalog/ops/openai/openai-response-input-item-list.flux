op openai-response-input-item-list(response_id: String, limit: Number) -> Any
  description "List input items retained for one stored response with a bounded integer limit"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.openai.com"
  url = fmt("{base}/v1/responses/{response_id}/input_items")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
