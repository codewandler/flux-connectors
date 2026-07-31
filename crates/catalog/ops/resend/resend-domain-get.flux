op resend-domain-get(domain_id: String) -> Any
  description "Read one sending domain, including the DNS records it needs and the status of each. This is the diagnostic for a domain that will not verify, and therefore for a send refused with an invalid-from error"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.resend.com"
  url = fmt("{base}/domains/{domain_id}")
  User_Agent = "flux-connectors"
  response = http.request(headers: { "User-Agent": User_Agent }, method: "GET", url)
  return response
