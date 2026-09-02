/**
 * Browser compatibility path; the SDK owns the checked live-devnet operator
 * surface.
 *
 * THIS FILE WAS A 377-LINE FORK, and the fork authenticated the deployment
 * more weakly than its owner did. The SDK's copy binds every role's upgrade
 * AUTHORITY to the authority the activation cache's own artifact release names,
 * requires the five activated artifacts to join the preset's Program,
 * ProgramData and deployment slot, requires one shared retained authority
 * across the generation, refuses an observation that predates its finalized
 * floor, and reports `routeSpecificReleaseAdmission` so no caller can promote
 * "the programs match" into "this route is admitted". The browser copy did none
 * of that, and the browser is the half a stranger uses.
 *
 * That gap is the same class as the closed-cohort finding of 2026-09-02: a
 * check that asks a weaker question than the one that matters passes on exactly
 * the state it should catch. The fork is deleted rather than deepened, because
 * two implementations of "is this deployment the one we think it is" is one
 * more than the number of correct answers.
 */
export * from '@dclutch/sdk/operatorSurface';
