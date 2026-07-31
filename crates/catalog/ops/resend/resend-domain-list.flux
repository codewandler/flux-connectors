op resend-domain-list -> Any
  description "List every sending domain on this account with its verification status. Also this connector's `verify` — a bounded read that runs unattended and needs nothing configured beyond the credential"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.resend.com"
  url = fmt("{base}/domains")
  User_Agent = "flux-connectors"
  response = http.request(headers: { "User-Agent": User_Agent }, method: "GET", url)
  return response
