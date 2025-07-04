import { type ComponentType, Suspense } from "react";

const withSuspense =
	<T extends object>(
		Component: ComponentType<T>,
		fallback?: React.ReactNode | null,
	) =>
	// eslint-disable-next-line react/display-name
	(props: T) => (
		<Suspense fallback={fallback ?? null}>
			<Component {...props} />
		</Suspense>
	);

/*
const withNode =
  <T,G>(Component: ComponentType<G>, fallback?: ReactElement | null) =>
  // eslint-disable-next-line react/display-name
  (props: T & { id: string }) =>
    (
      <Suspense fallback={fallback ?? null}>
        <Node Component={Component} {...props} />
      </Suspense>
    );
*/

export { withSuspense };
