import { type Session, SessionContext } from "hooks";
import type React from "react";
import { useState } from "react";

const SessionProvider = ({ children }: { children: React.ReactNode }) => {
	const [session, setSession] = useState<Session | null>(null);

	return (
		<SessionContext.Provider value={[session, setSession]}>
			{children}
		</SessionContext.Provider>
	);
};

export default SessionProvider;
