op babelforce-get-settings-for-app-customer-logging -> Any
  description "Get customer.logging settings"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/app/customer.logging")
  response = http.request(method: "GET", url)
  return response
