import { Component, ComponentType, Suspense } from 'react';
import { useNode } from 'hooks';

const withSuspense =
  <T extends object,>(Component: ComponentType<T>, fallback?: JSX.Element | null) =>
  // eslint-disable-next-line react/display-name
  (props: T) =>
    (
      <Suspense fallback={fallback ?? null}>
        <Component {...props} />
      </Suspense>
    );

const Node = <T, G>(props: T & { id: string; Component: ComponentType<G> }) => {
  const id = props.id;
  const node = useNode({ id });
  return <Component node={node} {...props} />;
};

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
