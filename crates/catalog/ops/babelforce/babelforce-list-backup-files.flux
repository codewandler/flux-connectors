op babelforce-list-backup-files -> Any
  description "List backup files"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/backups")
  response = http.request(method: "GET", url)
  return response
