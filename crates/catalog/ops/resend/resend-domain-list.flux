op resend-domain-list -> Any
  description "List every sending domain on this account with its verification status. Also this connector's `verify` — a bounded read that runs unattended and needs nothing configured beyond the credential"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.resend.com"
  url = fmt("{base}/domains")
  response = http.request(method: "GET", url)
  return response
