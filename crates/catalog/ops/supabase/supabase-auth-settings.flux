op supabase-auth-settings -> Any
  description "Read which sign-in methods this project has enabled, and whether self-service sign-up is open. Takes no argument and returns configuration only — no user data and no identities. Use it to decide whether a flow can offer a given provider before it tries"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{project_ref}.supabase.co"
  url = fmt("{base}/auth/v1/settings")
  response = http.request(method: "GET", url)
  return response
