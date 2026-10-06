import type { ReactNode } from "react";
import type { AnyZodObject } from "remotion";
import { Composition } from "remotion";

import "#/app/styles.css";
import type { MouIkkaiProps } from "#/pages/mou-ikkai";
import { MouIkkai } from "#/pages/mou-ikkai";
import { DURATION_IN_FRAMES, FPS } from "#/shared/song";

const WIDTH = 1920;
const HEIGHT = 1080;
const defaultProps: MouIkkaiProps = { audio: "mou-ikkai-mix.wav" };

const Root = (): ReactNode => (
  <Composition<AnyZodObject, MouIkkaiProps>
    id="MouIkkai"
    component={MouIkkai}
    durationInFrames={DURATION_IN_FRAMES}
    fps={FPS}
    width={WIDTH}
    height={HEIGHT}
    defaultProps={defaultProps}
  />
);

export { Root };
