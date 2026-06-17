"""
Configuration module for Tango Signaling Server.
"""

import os
from typing import Optional


class Settings:
    """Application settings loaded from environment variables."""
    
    # Server configuration
    SERVER_HOST: str = os.getenv("SERVER_HOST", "0.0.0.0")
    SERVER_PORT: int = int(os.getenv("SERVER_PORT", "8000"))
    
    # TURN server configuration
    TURN_ADDR: Optional[str] = os.getenv("TURN_ADDR")
    TURN_USER: Optional[str] = os.getenv("TURN_USER")
    TURN_CREDENTIAL: Optional[str] = os.getenv("TURN_CREDENTIAL")
    
    # Cloudflare TURN service
    CLOUDFLARE_TURN_SERVICE_ID: Optional[str] = os.getenv("CLOUDFLARE_TURN_SERVICE_ID")
    CLOUDFLARE_TURN_SERVICE_API_TOKEN: Optional[str] = os.getenv("CLOUDFLARE_TURN_SERVICE_API_TOKEN")
    
    # Application settings
    DEBUG: bool = os.getenv("DEBUG", "false").lower() == "true"
    LOG_LEVEL: str = os.getenv("LOG_LEVEL", "INFO")


settings = Settings()
