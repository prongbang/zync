import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { Toaster } from "@workspace/ui/components/toast"

import "@workspace/ui/globals.css"
import "./zync-theme.css"
import { AuthGate } from "./AuthGate.tsx"
import { ThemeProvider } from "@/components/theme-provider.tsx"

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider defaultTheme="dark">
      <Toaster />
      <AuthGate />
    </ThemeProvider>
  </StrictMode>
)
