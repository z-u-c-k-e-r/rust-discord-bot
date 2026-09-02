return {
  metadata = {
    name = "core",
    version = "0.1.0",
    description = "Core diagnostics and product information."
  },
  commands = {
    {
      name = "ping",
      description = "Sprawdza, czy bot i silnik Lua działają.",
      dm_permission = true,
      handler = function(ctx)
        return {
          content = "Pong. Rust działa, Lua odpowiada. Użytkownik: " .. ctx.user_name,
          ephemeral = false
        }
      end
    },
    {
      name = "about",
      description = "Pokazuje informacje o platformie ZuckerBot.",
      dm_permission = true,
      handler = function(_)
        return {
          content = table.concat({
            "**ZuckerBot 0.1.0**",
            "Bezpieczny rdzeń w Rust, rozszerzenia w Lua 5.4 i panel WWW.",
            "Aktualny etap: fundament platformy, rejestr komend, sandbox Lua oraz warstwa głosowa."
          }, "\n"),
          ephemeral = true
        }
      end
    }
  }
}
