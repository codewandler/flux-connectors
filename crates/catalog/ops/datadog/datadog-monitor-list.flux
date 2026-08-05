op datadog-monitor-list -> Any
  description "List every monitor visible to this API key. Also this connector's `verify` — a bounded read that runs unattended, needing no argument"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.datadoghq.com"
  url = fmt("{base}/api/v1/monitor")
  response = http.request(method: "GET", url)
  return response
