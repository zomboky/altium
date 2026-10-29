import { PORT, prepare, serve, waitForServer } from './altium.ts'

export default async function setup() {
  // Another server on the port would answer instead of ours.
  const taken = await fetch(`http://127.0.0.1:${PORT}/api/health`).then(
    () => true,
    () => false,
  )
  if (taken) throw new Error(`le port ${PORT} est déjà pris : arrête ce serveur ou choisis ALTIUM_E2E_PORT`)
  prepare()
  const server = serve()
  await waitForServer()
  return async () => {
    server.kill()
  }
}
